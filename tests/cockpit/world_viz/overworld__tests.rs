use super::ink::{BLACK, PALETTE, is_signal, palette_index, rgb};
use super::*;

/// The realm screen that holds a place.
fn screen_of(place: Place) -> View {
    let (sx, sy) = place.screen();
    View::screen(sx, sy)
}

fn busy(tick: u32) -> Scene {
    let mut s = Scene::resting();
    s.tier = 2;
    s.active = Some(Place::Smithy);
    s.tool = Some(Tool::Hammer);
    s.knight = Knight::at_place(Place::Smithy);
    s.council = vec![(0, '1'), (2, '7'), (4, '2')];
    s.quest = Some((4, 12));
    s.joust = Some(Joust {
        red: "glm".to_string(),
        blue: "dsk".to_string(),
        red_score: 3,
        blue_score: 2,
        charge: 0.5,
    });
    s.chapel_lit = true;
    s.cottages = [true, false, true];
    s.forge_hot = true;
    s.wards = vec![
        Ward {
            name: "angelX".to_string(),
            banner: '1',
            lit: true,
        },
        Ward {
            name: "dotmax".to_string(),
            banner: '4',
            lit: false,
        },
    ];
    s.muster = [
        (SoldierKind::Verify, SoldierState::Returned),
        (SoldierKind::Verify, SoldierState::Running),
        (SoldierKind::Verify, SoldierState::Running),
        (SoldierKind::Verify, SoldierState::Failed),
        (SoldierKind::Verify, SoldierState::Running),
        (SoldierKind::Verify, SoldierState::Cut),
        (SoldierKind::Judge, SoldierState::Running),
        (SoldierKind::Judge, SoldierState::Returned),
    ]
    .into_iter()
    .map(|(kind, state)| Soldier { kind, state })
    .collect();
    s.tick = tick;
    s
}

#[test]
fn every_screen_is_sixteen_by_eleven() {
    for sy in 0..map::AUTHORED_Y {
        for sx in 0..map::AUTHORED_X {
            let rows = Realm::authored_rows(sx, sy);
            for (i, row) in rows.iter().enumerate() {
                assert_eq!(row.len(), SCREEN_W as usize, "screen ({sx},{sy}) row {i}");
            }
        }
    }
}

#[test]
fn roads_that_leave_a_screen_arrive_on_the_next() {
    let realm = Realm::get();
    let road = |t: u8| matches!(t, b'=' | b':' | b'H');
    for y in 0..MAP_H {
        for sx in 1..map::SCREENS_X {
            let x = sx * SCREEN_W;
            assert_eq!(
                road(realm.at(x - 1, y)),
                road(realm.at(x, y)),
                "road seam at x={x} y={y}"
            );
        }
    }
    for x in 0..MAP_W {
        for sy in 1..map::SCREENS_Y {
            let y = sy * SCREEN_H;
            assert_eq!(
                road(realm.at(x, y - 1)),
                road(realm.at(x, y)),
                "road seam at x={x} y={y}"
            );
        }
    }
}

#[test]
fn every_place_has_ground_to_stand_on() {
    let realm = Realm::get();
    for place in Place::ALL {
        let (tx, ty) = place.stand_world();
        assert!(
            map::walkable(realm.at(tx, ty)),
            "{place:?} stand ({tx},{ty}) is not walkable"
        );
    }
}

#[test]
fn frames_stay_on_the_realm_palette() {
    let f = frame(&busy(0));
    for c in f.pixels() {
        assert!(
            c == BLACK || palette_index(c).is_some(),
            "off-palette pixel {c:?}"
        );
    }
    let realm = render_view(&busy(3), View::realm());
    for c in realm.pixels() {
        assert!(
            c == BLACK || palette_index(c).is_some(),
            "off-palette pixel {c:?}"
        );
    }
}

#[test]
fn the_same_scene_renders_the_same_bytes() {
    assert_eq!(frame(&busy(5)).rgb_bytes(), frame(&busy(5)).rgb_bytes());
    assert_ne!(
        frame(&busy(1)).rgb_bytes(),
        frame(&busy(2)).rgb_bytes(),
        "ticks animate the forge and the beacon"
    );
}

#[test]
fn signal_inks_never_dim_and_shade_stays_in_its_bank() {
    for &v in PALETTE.iter() {
        let c = rgb(v);
        let bank = palette_index(c).unwrap() / ink::BANK;
        for n in 0..7 {
            let d = light::step_down(c, n);
            if is_signal(c) {
                assert_eq!(d, c, "signal {c:?} dimmed");
            } else if d != BLACK {
                assert_eq!(
                    palette_index(d).unwrap() / ink::BANK,
                    bank,
                    "{c:?} left its bank"
                );
            }
        }
    }
}

#[test]
fn light_means_activity() {
    let rest = scene::stage(&Scene::resting());
    assert!(
        rest.beacons.is_empty(),
        "a resting realm marks nothing live"
    );
    let work = scene::stage(&busy(0));
    assert_eq!(work.beacons.len(), 1, "one live place, one beacon");
    assert!(
        work.lights.len() > rest.lights.len(),
        "live work casts light"
    );
    // Only the keep's gate torches and the knight light a resting realm.
    assert_eq!(rest.lights.len(), 3);
}

#[test]
fn renown_builds_the_town() {
    let mut s = Scene::resting();
    let bare = scene::stage(&s).props.len();
    s.tier = 8;
    let grown = scene::stage(&s).props.len();
    assert!(
        grown >= bare + 10,
        "tier 8 adds garden, well, lanterns, docks, mill, market, turrets"
    );
}

#[test]
fn the_knight_frames_his_own_screen() {
    let s = busy(0);
    let f = frame(&s);
    assert_eq!((f.w, f.h), (SCREEN_W * TILE, SCREEN_H * TILE));
    assert_eq!(
        (f.w as u32, f.h as u32),
        (FRAME_W, FRAME_H),
        "no HUD band: the frame is the world"
    );
    assert_eq!(
        View::screen_at(s.knight.x, s.knight.y),
        screen_of(Place::Keep)
    );
    assert_eq!(Place::Lists.screen(), (4, 2));
    assert_eq!(Place::Keep.screen(), (2, 2), "the town sits mid-realm");
}

/// Offline renders for review: `ANGEL_OVERWORLD_SHOTS=<dir> cargo test ... -- --ignored`.
#[test]
#[ignore]
fn write_overworld_shots() {
    let Some(dir) = std::env::var_os("ANGEL_OVERWORLD_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(name), out).unwrap();
    };
    for t in 0..16 {
        let mut s = busy(t);
        if let Some(j) = s.joust.as_mut() {
            j.charge = t as f32 / 15.0;
        }
        save(
            &format!("town_{t:02}.ppm"),
            &frame_at(&s, screen_of(Place::Keep)),
        );
        save(
            &format!("lists_{t:02}.ppm"),
            &frame_at(&s, screen_of(Place::Lists)),
        );
    }
    save("realm.ppm", &render_view(&busy(8), View::realm()));
    let mut grown = busy(8);
    grown.tier = 8;
    save("town_tier8.ppm", &frame_at(&grown, screen_of(Place::Keep)));
    save("resting.ppm", &frame(&Scene::resting()));
    let world = World::new(7);
    let mut arrived = busy(4);
    arrived.glass = Some(world.overworld_plate_glass(crate::stage::world_viz::Building::Smithy));
    save(
        "glass_plate.ppm",
        &frame_at(&arrived, screen_of(Place::Keep)),
    );
    let mut riding = busy(4);
    riding.knight.walking = true;
    riding.glass = Some(world.overworld_ride_glass(crate::stage::world_viz::Building::Chapel));
    save("glass_ride.ppm", &frame_at(&riding, screen_of(Place::Keep)));
    for (name, sky) in [
        ("rain", Weather::Rain),
        ("storm", Weather::Storm),
        ("rainbow", Weather::Rainbow),
        ("clouds", Weather::Clouds),
    ] {
        let mut s = busy(17);
        s.weather = sky;
        save(
            &format!("sky_{name}.ppm"),
            &frame_at(&s, screen_of(Place::Keep)),
        );
    }
    let mut win = busy(9);
    win.fireworks = true;
    save("fireworks.ppm", &frame_at(&win, screen_of(Place::Keep)));
    let mut quest = busy(6);
    quest.knight = Knight::at_place(Place::DragonKeep);
    quest.party = vec![(640.0, 110.0), (628.0, 104.0), (616.0, 98.0)];
    quest.dragon = true;
    save(
        "dragon.ppm",
        &frame_at(&quest, screen_of(Place::DragonKeep)),
    );
    let mut mine = busy(6);
    mine.knight = Knight::at_place(Place::Mines);
    mine.chests = (3, 2);
    save("mines.ppm", &frame_at(&mine, screen_of(Place::Mines)));
    let mut bog = busy(6);
    bog.knight = Knight::at_place(Place::Swamp);
    bog.wisps = true;
    save("swamp.ppm", &frame_at(&bog, screen_of(Place::Swamp)));
    save("wards.ppm", &frame_at(&busy(6), screen_of(Place::Wards)));
    // Pane-shaped views at the locked scale: a tall narrow pane, a wide one.
    save("pane_tall.ppm", &frame_sized(&busy(6), 170, 330));
    for (name, (sx, sy)) in [
        ("waterfall", (2, 1)),
        ("lava", (5, 1)),
        ("wild_forest", (1, 1)),
    ] {
        let mut s = busy(6);
        s.camera = (
            ((sx * SCREEN_W + 8) * TILE) as f32,
            ((sy * SCREEN_H + 5) * TILE) as f32,
        );
        save(&format!("{name}.ppm"), &frame_sized(&s, 256, 176));
    }
    let mut reading = busy(6);
    reading.active = Some(Place::Scriptorium);
    reading.tool = Some(Tool::Book);
    reading.knight = Knight::at_place(Place::Scriptorium);
    reading.muster.clear();
    save(
        "reading.ppm",
        &frame_at(&reading, screen_of(Place::Scriptorium)),
    );
    save("pane_wide.ppm", &frame_sized(&busy(6), 420, 200));
}

#[test]
fn the_palette_is_the_realm_master_palette() {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("../../../cockpit/assets/realm/palette.json")).unwrap();
    let banks = ["structure", "foliage", "timber", "stone", "signal"];
    for (b, name) in banks.iter().enumerate() {
        let colors = json["banks"][name]["colors"].as_array().unwrap();
        assert_eq!(json["banks"][name]["index"].as_u64(), Some(b as u64));
        for (i, hex) in colors.iter().enumerate() {
            let hex = hex.as_str().unwrap().trim_start_matches('#');
            let v = u32::from_str_radix(hex, 16).unwrap();
            assert_eq!(PALETTE[b * ink::BANK + i], v, "{name}[{i}]");
        }
    }
}

// ─── the live map ────────────────────────────────────────────────────────────

use crate::agent::harness::ToolEventId;
use crate::stage::world_viz::World;

fn adjacent(a: (i32, i32), b: (i32, i32)) -> bool {
    (a.0 - b.0).abs() + (a.1 - b.1).abs() == 1
}

#[test]
fn every_place_is_reachable_on_foot_from_the_keep() {
    for place in Place::ALL {
        let path = live::route(Place::Keep.stand_world(), place.stand_world());
        if place == Place::Keep {
            assert!(path.is_empty());
            continue;
        }
        assert_eq!(path.last(), Some(&place.stand_world()), "{place:?}");
        let mut at = Place::Keep.stand_world();
        for &step in &path {
            assert!(
                adjacent(at, step),
                "{place:?}: {at:?} -> {step:?} is not one step"
            );
            at = step;
        }
    }
}

#[test]
fn town_errands_keep_to_the_roads() {
    let realm = Realm::get();
    let path = live::route(Place::Keep.stand_world(), Place::Smithy.stand_world());
    assert!(
        path.iter()
            .all(|&(x, y)| matches!(realm.at(x, y), b'=' | b':')),
        "keep to smithy should stay on road and cobble: {path:?}"
    );
}

#[test]
fn the_walker_arrives_and_stops() {
    let mut w = Walker::default();
    assert_eq!(w.knight(), Knight::at_place(Place::Keep));
    w.toward(Place::Smithy);
    assert!(w.knight().walking, "a new goal sets him walking");
    for _ in 0..400 {
        w.toward(Place::Smithy);
    }
    assert_eq!(w.knight(), Knight::at_place(Place::Smithy));
    assert!(!w.knight().walking);
}

#[test]
fn the_live_scene_reads_the_world() {
    let mut world = World::new(7);
    let rest = world.overworld_scene();
    assert_eq!(rest.active, None);
    assert_eq!(rest.knight, Knight::at_place(Place::Keep));

    world.note_tool_call_event(ToolEventId("r1".to_string()), "read_file", "src/lib.rs");
    let reading = world.overworld_scene();
    assert_eq!(reading.active, Some(Place::Scriptorium));
    assert_eq!(reading.tool, Some(Tool::Book));
    for _ in 0..600 {
        world.tick();
    }
    assert_eq!(world.overworld_goal(), Place::Scriptorium);
    assert_eq!(
        world.overworld_scene().knight,
        Knight::at_place(Place::Scriptorium)
    );
}

#[test]
fn a_trial_sends_the_wagon_and_the_knight_to_the_lists() {
    use crate::agent::harness::{ExecutionOutcome, ToolOutcome, VerificationOutcome};
    let mut world = World::new(7);
    let trial = ToolEventId("t1".to_string());
    world.note_tool_call_event(trial.clone(), "shell", "cargo test --workspace");
    assert_eq!(world.overworld_goal(), Place::Lists);
    let s = world.overworld_scene();
    assert_eq!(s.active, Some(Place::Lists));
    assert_eq!(s.tool, Some(Tool::Sword));
    assert_eq!(s.wayfarers.len(), 1, "the cargo wagon sets out");
    for _ in 0..600 {
        world.tick();
    }
    assert_eq!(
        world.overworld_scene().knight,
        Knight::at_place(Place::Lists),
        "he waits at the Lists for the verdict"
    );
    world.note_tool_result_event(
        &trial,
        "shell",
        "test result: FAILED",
        ToolOutcome {
            execution: ExecutionOutcome::Failed,
            verification: VerificationOutcome::Failed,
        },
    );
    world.tick();
    let s = world.overworld_scene();
    assert_eq!(
        s.record.pennants,
        vec![false],
        "an amber pennant, not a red one"
    );
    assert_eq!(s.wayfarers[0].verdict, Some(false));
    assert_ne!(world.overworld_goal(), Place::Lists);

    world.note_tool_call_event(ToolEventId("c1".to_string()), "shell", "git commit -m x");
    world.turn_ended(false);
    for _ in 0..2_000 {
        world.tick();
    }
    let s = world.overworld_scene();
    assert!(s.wayfarers.is_empty(), "everyone is home after the turn");
    assert_eq!(s.record.ravens, 0, "an unfinished commit earns no raven");
    assert_eq!(
        s.record.pennants,
        vec![false],
        "the record outlives the turn"
    );
}

#[test]
fn a_loop_between_rounds_rests_at_the_quintain() {
    let mut world = World::new(7);
    world.loop_active = true;
    assert_eq!(world.overworld_goal(), Place::Lists);
    world.loop_iteration = 3;
    let s = world.overworld_scene();
    assert_eq!(s.quest.map(|(i, _)| i), Some(3));
}

#[test]
fn night_sinks_below_the_dusk_mood() {
    assert!((live::ambient_for(1.0) - light::DUSK).abs() < 1e-6);
    assert!(live::ambient_for(0.55) < light::DUSK - 0.1);
}

// ─── painting it ─────────────────────────────────────────────────────────────

#[test]
fn the_map_paces_its_frames() {
    let (mut a, mut b, mut c, mut r) = (busy(13), busy(19), busy(20), busy(33));
    pace(&mut a, false);
    pace(&mut b, false);
    pace(&mut c, false);
    pace(&mut r, true);
    assert_eq!(a.tick, 1, "ambient motion steps about four times a second");
    assert_eq!(a.key(), b.key(), "ticks inside one step share a frame");
    assert_ne!(a.key(), c.key(), "the next step is a new frame");
    assert_eq!(r.tick, 1, "a running turn halves the cadence");
    let mut moved = busy(13);
    moved.knight.x += 0.3;
    pace(&mut moved, false);
    assert_eq!(moved.key(), a.key(), "sub-pixel drift is not a new frame");
}

#[test]
fn lazy_frames_render_rgba_on_the_worker() {
    let lazy = LazyFrame::new(busy(0), 200, 150, 3);
    assert_eq!(lazy.size(), (600, 450));
    let bytes = lazy.as_ref();
    assert_eq!(bytes.len(), 600 * 450 * 4);
    assert!(bytes.chunks_exact(4).all(|px| px[3] == 255));
    let native = frame_sized(&busy(0), 200, 150);
    assert_eq!(bytes, native.rgba_scaled(3).as_slice());
    // Whole pixels: every 3x3 block is one map pixel.
    let at = |x: usize, y: usize| &bytes[(y * 600 + x) * 4..(y * 600 + x) * 4 + 4];
    assert_eq!(at(3 * 77, 3 * 41), at(3 * 77 + 2, 3 * 41 + 2));
}

#[test]
fn the_realm_route_keeps_travel_on_the_map() {
    use crate::stage::world_viz::Building;
    use crate::ui::scryglass::{Scryglass, StageRoute, StageSurface};
    let _env = crate::tests::env_lock();
    let mut stage = Scryglass::default();
    stage.sync_arrival(Some(Building::Keep));
    stage.begin_journey(ToolEventId("walk".into()), Building::Smithy, false);
    assert_eq!(stage.controller.route(), StageRoute::Realm);
    {
        let _map = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_MAP");
        assert!(map_enabled());
        assert_eq!(
            stage.controller.resolved_scene(false, false, true),
            StageSurface::WorldMap,
            "the knight walks on the map, even when a quest owns the pane"
        );
    }
    let _ride = crate::tests::TestEnvGuard::set("ANGEL_WORLD_MAP", "3d");
    assert!(!map_enabled());
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::WorldFirstPerson
    );
}

// ─── the scrying glass ───────────────────────────────────────────────────────

#[test]
fn glass_pictures_are_snapped_to_the_plain_palette() {
    let rgba: Vec<u8> = (0..64u32 * 48)
        .flat_map(|i| {
            [
                (i % 251) as u8,
                (i * 7 % 253) as u8,
                (i * 13 % 241) as u8,
                255,
            ]
        })
        .collect();
    let pic = picture_from_rgba(&rgba, 64, 48);
    assert_eq!((pic.w, pic.h), (GLASS_W, GLASS_H));
    for c in pic.pixels() {
        assert!(
            c == BLACK || (palette_index(c).is_some() && !is_signal(c)),
            "{c:?}"
        );
    }
    assert_eq!(
        picture_from_rgba(&[], 0, 0)
            .pixels()
            .filter(|&c| c != BLACK)
            .count(),
        0
    );
}

#[test]
fn a_glass_frames_its_place_and_tethers_to_it() {
    // The ride and the plates read assets through env-dependent runtime paths.
    let _env = crate::tests::env_lock();
    let world = World::new(7);
    let plate = world.overworld_plate_glass(crate::stage::world_viz::Building::Smithy);
    let colours: std::collections::HashSet<_> = plate.picture.pixels().collect();
    assert!(colours.len() > 8, "the smithy painting survives the glass");
    let bare = frame(&busy(0));
    let mut s = busy(0);
    s.glass = Some(plate);
    let framed = frame(&s);
    assert_ne!(bare.rgba_bytes(), framed.rgba_bytes());
    for c in framed.pixels() {
        assert!(
            c == BLACK || palette_index(c).is_some(),
            "off-palette {c:?}"
        );
    }
    // The smithy is on screen: the tether ends in a signal mark on it.
    let town = screen_of(Place::Smithy);
    let (tx, ty, tw, th) = Place::Smithy.footprint_world();
    let (ax, ay) = (
        tx * TILE + tw * TILE / 2 - town.x,
        ty * TILE + th * TILE / 2 - town.y,
    );
    assert_eq!(framed.get(ax, ay), ink::ink('3'));
    assert_ne!(
        s.key(),
        busy(0).key(),
        "a glass is part of the frame's identity"
    );
}

#[test]
fn the_ride_glass_shows_the_dotmax_saddle_view() {
    // The ride and the plates read assets through env-dependent runtime paths.
    let _env = crate::tests::env_lock();
    let world = World::new(7);
    let ride = world.overworld_ride_glass(crate::stage::world_viz::Building::Chapel);
    assert!(ride.live);
    let lit = ride.picture.pixels().filter(|&c| c != BLACK).count();
    assert!(
        lit > (GLASS_W * GLASS_H / 4) as usize,
        "the ride paints ({lit} px)"
    );
    let again = world.overworld_ride_glass(crate::stage::world_viz::Building::Chapel);
    assert!(
        std::sync::Arc::ptr_eq(&ride.picture, &again.picture),
        "same frame, cached"
    );
}

// ─── the world built out ─────────────────────────────────────────────────────

#[test]
fn the_sky_follows_the_weather_and_stays_on_the_palette() {
    let fair = frame(&busy(17));
    for sky in [
        Weather::Clouds,
        Weather::Rain,
        Weather::Storm,
        Weather::Rainbow,
    ] {
        let mut s = busy(17);
        s.weather = sky;
        let f = frame(&s);
        assert_ne!(
            f.rgba_bytes(),
            fair.rgba_bytes(),
            "{sky:?} changes the frame"
        );
        for c in f.pixels() {
            assert!(
                c == BLACK || palette_index(c).is_some(),
                "{sky:?}: off-palette {c:?}"
            );
        }
    }
    // No rain: wet weather is shade, never falling streaks.
    let mut wet = busy(17);
    wet.weather = Weather::Storm;
    let mut wet_later = busy(17);
    wet_later.weather = Weather::Storm;
    wet_later.tick = 18;
    let mut dry_later = busy(17);
    dry_later.weather = Weather::Clouds;
    dry_later.tick = 18;
    let changed = |a: &Img, b: &Img| a.pixels().zip(b.pixels()).filter(|(x, y)| x != y).count();
    let mut dry = busy(17);
    dry.weather = Weather::Clouds;
    assert_eq!(
        changed(&frame(&wet), &frame(&wet_later)),
        changed(&frame(&dry), &frame(&dry_later)),
        "a storm animates exactly like plain cloud"
    );
    let mut drizzle = busy(17);
    drizzle.weather = Weather::Drizzle;
    assert_eq!(
        frame(&drizzle).rgba_bytes(),
        fair.rgba_bytes(),
        "drizzle draws nothing"
    );
    let mut won = busy(9);
    won.fireworks = true;
    assert_ne!(frame(&won).rgba_bytes(), frame(&busy(9)).rgba_bytes());
}

#[test]
fn the_party_walks_in_the_knights_steps() {
    let mut w = Walker::default();
    for _ in 0..200 {
        w.toward(Place::Scriptorium);
    }
    let followers = w.followers(3);
    assert_eq!(followers.len(), 3);
    let k = w.knight();
    for (i, &(x, y)) in followers.iter().enumerate() {
        let d = ((x - k.x).powi(2) + (y - k.y).powi(2)).sqrt();
        assert!(
            d > 4.0 * (i + 1) as f32,
            "follower {i} keeps its distance ({d})"
        );
        assert!(d < 60.0 * (i + 1) as f32, "follower {i} keeps up ({d})");
    }
}

#[test]
fn a_two_seat_stage_is_a_duel_at_the_lists() {
    use crate::ui::viz::agentviz::SeatState;
    let mut world = World::new(7);
    let seats = vec!["glm-5.3".to_string(), "deepseek".to_string()];
    world.note_duel(11, &seats, &[]);
    let j = world.overworld_scene().joust.expect("a duel rides");
    assert_eq!((j.red.as_str(), j.blue.as_str()), ("glm-5.3", "deepseek"));
    assert_eq!((j.red_score, j.blue_score), (0, 0));
    world.note_duel(11, &seats, &[SeatState::Returned, SeatState::Running]);
    world.note_duel(11, &seats, &[SeatState::Returned, SeatState::Failed]);
    let j = world.overworld_scene().joust.expect("still riding");
    assert_eq!((j.red_score, j.blue_score), (1, 0), "a result scores once");
    assert_eq!(j.charge, 1.0, "the pass is over");
    world.note_duel(12, &["a".into(), "b".into(), "c".into()], &[]);
    assert!(
        world.overworld_scene().joust.is_none(),
        "three seats muster, they do not joust"
    );
    world.note_duel(13, &seats, &[SeatState::Running, SeatState::Returned]);
    let j = world.overworld_scene().joust.unwrap();
    assert_eq!(
        (j.red_score, j.blue_score),
        (1, 1),
        "the tally is the session's ladder"
    );
}

#[test]
fn the_adventure_shows_what_the_quest_holds() {
    let base = scene::stage(&busy(3)).props.len();
    let mut s = busy(3);
    s.dragon = true;
    s.chests = (2, 1);
    s.wisps = true;
    s.party = vec![(300.0, 300.0), (290.0, 300.0)];
    let staged = scene::stage(&s);
    assert_eq!(
        staged.props.len(),
        base + 1 + 3 + 3 + 2,
        "dragon, chests, wisps, party"
    );
    assert!(
        staged.lights.len() > scene::stage(&busy(3)).lights.len(),
        "fire and wisps glow"
    );
    let resting = Scene::resting();
    assert!(!resting.dragon && !resting.wisps && resting.party.is_empty());
}

// ─── the locked camera ───────────────────────────────────────────────────────

#[test]
fn views_keep_inside_the_realm_and_centre_what_they_outgrow() {
    let v = View::around(10.0, 10.0, 200, 120);
    assert_eq!((v.x, v.y), (0, 0), "clamped at the north-west corner");
    let v = View::around(5000.0, 5000.0, 200, 120);
    assert_eq!(
        (v.x + v.w, v.y + v.h),
        (MAP_W * TILE, MAP_H * TILE),
        "clamped at the south-east"
    );
    let v = View::around(400.0, 300.0, 200, 120);
    assert_eq!((v.x, v.y), (300, 240), "centred on the camera");
    let wide = View::around(400.0, 300.0, MAP_W * TILE + 100, 120);
    assert_eq!(wide.x, -50, "a view wider than the realm centres it");
}

#[test]
fn a_bigger_pane_shows_more_realm_at_the_same_scale() {
    let s = busy(0);
    let small = frame_sized(&s, 160, 100);
    let big = frame_sized(&s, 480, 300);
    assert_eq!((small.w, small.h), (160, 100));
    assert_eq!((big.w, big.h), (480, 300));
    // The knight sits at the same scale: his sprite pixels are unchanged.
    let knight_px = |f: &Img| f.pixels().filter(|&c| Some(c) == ink::ink('7')).count();
    assert!(knight_px(&small) > 0 && knight_px(&big) >= knight_px(&small));
    // Past the realm's edge is black paper.
    let whole = frame_sized(&s, MAP_W * TILE + 40, MAP_H * TILE + 40);
    for y in 0..whole.h {
        for x in 0..20 {
            assert_eq!(whole.get(x, y), Some(BLACK), "margin ({x},{y})");
        }
    }
}

#[test]
fn the_map_scale_follows_the_text() {
    assert_eq!(map_scale(8), 1);
    assert_eq!(map_scale(12), 1);
    assert_eq!(map_scale(20), 2);
    assert_eq!(map_scale(26), 2);
    assert_eq!(map_scale(32), 3);
    assert_eq!(map_scale(0), 1);
}

#[test]
fn the_camera_holds_inside_its_dead_zone_and_follows_out_of_it() {
    use super::live::follow;
    assert_eq!(follow((100.0, 100.0), (130.0, 110.0)), (100.0, 100.0));
    assert_eq!(follow((100.0, 100.0), (160.0, 100.0)), (120.0, 100.0));
    assert_eq!(follow((100.0, 100.0), (100.0, 40.0)), (100.0, 68.0));
    let mut w = Walker::default();
    let start = w.camera();
    for _ in 0..400 {
        w.toward(Place::Scriptorium);
    }
    let (cx, cy) = w.camera();
    let k = w.knight();
    assert!(
        cx != start.0 || cy != start.1,
        "a long walk moves the camera"
    );
    assert!(
        (k.x - cx).abs() <= 40.0 && (k.y - 8.0 - cy).abs() <= 28.0,
        "and keeps him in the dead zone"
    );
}

// ─── slow, warm ambiance ─────────────────────────────────────────────────────

#[test]
fn the_realm_has_streams_falls_and_lava() {
    let realm = Realm::get();
    let count = |t: u8| {
        (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .filter(|&(x, y)| realm.at(x, y) == t)
            .count()
    };
    assert!(count(b'w') > 4, "streams run");
    assert!(count(b'f') > 0, "somewhere a stream falls off the rock");
    assert!(count(b'L') > 4, "lava in the ash wastes");
    // The hills just north of Castle Town carry a waterfall.
    let (sx, sy) = (2, 1);
    let falls = (0..SCREEN_H)
        .flat_map(|y| (0..SCREEN_W).map(move |x| (x, y)))
        .filter(|&(x, y)| matches!(realm.at(sx * SCREEN_W + x, sy * SCREEN_H + y), b'f' | b'w'))
        .count();
    assert!(falls > 3, "a stream above the town ({falls} tiles)");
}

#[test]
fn water_and_lava_move_slowly() {
    let mut s = busy(6);
    s.camera = (
        ((2 * SCREEN_W + 8) * TILE) as f32,
        ((SCREEN_H + 5) * TILE) as f32,
    );
    let a = frame_sized(&s, 256, 176);
    s.tick += 1;
    let b = frame_sized(&s, 256, 176);
    let moved = a.pixels().zip(b.pixels()).filter(|(x, y)| x != y).count();
    assert!(moved > 0, "the water moves");
    assert!(
        moved < (256 * 176) / 10,
        "slowly: most of the frame stands still ({moved} px)"
    );
}

#[test]
fn reading_is_done_by_lantern() {
    let mut s = busy(0);
    s.tool = Some(Tool::Book);
    s.knight = Knight::at_place(Place::Scriptorium);
    let staged = scene::stage(&s);
    assert!(
        staged.props.iter().any(|p| p.img.w == 22),
        "the knight sits with his book"
    );
    assert!(
        staged.cues.is_empty(),
        "the book in his lap says it; no bubble"
    );
    let k = s.knight;
    assert!(
        staged
            .lights
            .iter()
            .any(|l| l.fire && (l.x - k.x - 11.0).abs() < 1.0),
        "a warm lantern burns beside him"
    );
    let mut walking = s.clone();
    walking.knight.walking = true;
    assert!(
        scene::stage(&walking).props.iter().all(|p| p.img.w != 22),
        "no reading on the road"
    );
}

#[test]
fn the_map_shows_the_published_pose() {
    let mut w = Walker::default();
    let start = w.knight();
    w.toward(Place::Smithy);
    w.toward(Place::Smithy);
    assert_ne!(w.knight(), start, "he walks every tick");
    assert_eq!(
        w.shown_knight(),
        start,
        "the map shows him only when published"
    );
    w.publish();
    assert_eq!(w.shown_knight(), w.knight());
}

/// A scripted session acted out on the map, for review:
/// `ANGEL_OVERWORLD_SHOTS=<dir> cargo test write_deed_shots -- --ignored`.
#[test]
#[ignore]
fn write_deed_shots() {
    use crate::agent::harness::{ExecutionOutcome, ToolEventId, ToolOutcome, VerificationOutcome};
    let Some(dir) = std::env::var_os("ANGEL_OVERWORLD_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(name), out).unwrap();
    };
    let outcome = |passed: bool| ToolOutcome {
        execution: if passed {
            ExecutionOutcome::Succeeded
        } else {
            ExecutionOutcome::Failed
        },
        verification: VerificationOutcome::NotApplicable,
    };
    let mut w = World::new(7);
    let mut n = 0;
    let mut call = |w: &mut World, name: &str, args: &str| {
        n += 1;
        let id = ToolEventId(format!("shot-{n}"));
        w.note_tool_call_event(id.clone(), name, args);
        id
    };
    let ticks = |w: &mut World, k: u32| {
        for _ in 0..k {
            w.tick();
        }
    };
    let shot = |w: &World, name: &str, focus: Option<(f32, f32)>| {
        let mut s = w.overworld_scene();
        if let Some(focus) = focus {
            s.camera = focus;
        }
        save(name, &frame_sized(&s, 320, 200));
    };
    let at = |p: Place| {
        let k = Knight::at_place(p);
        (k.x, k.y - 8.0)
    };

    for file in [
        "src/a.rs",
        "src/b.rs",
        "README.md",
        "docs/x.md",
        "src/c.rs",
        "Cargo.toml",
        "src/d.rs",
    ] {
        let id = call(&mut w, "read_file", &format!("path={file}"));
        ticks(&mut w, 4);
        w.note_tool_result_event(&id, "read_file", "ok", outcome(true));
    }
    for file in ["src/a.rs", "src/b.rs", "src/c.rs", "src/e.rs"] {
        let id = call(&mut w, "edit_file", &format!("path={file}"));
        ticks(&mut w, 3);
        w.note_tool_result_event(&id, "edit_file", "ok", outcome(true));
    }
    shot(&w, "d01_forge_sparks.ppm", Some(at(Place::Smithy)));
    ticks(&mut w, 30);
    shot(
        &w,
        "d02_scriptorium_books.ppm",
        Some(at(Place::Scriptorium)),
    );

    let trial = call(&mut w, "shell", "cd /repo && cargo test -p cockpit");
    ticks(&mut w, 90);
    let wagon = w
        .overworld_scene()
        .wayfarers
        .first()
        .map(|f| (f.x, f.y - 8.0));
    shot(&w, "d03_wagon_on_the_road.ppm", wagon);
    ticks(&mut w, 900);
    shot(&w, "d04_wagon_at_the_lists.ppm", Some(at(Place::Lists)));
    w.note_tool_result_event(&trial, "shell", "ok", outcome(true));
    ticks(&mut w, 20);
    shot(&w, "d05_trial_passed.ppm", Some(at(Place::Lists)));
    ticks(&mut w, 900);
    let second = call(&mut w, "cargo", "clippy --all-targets");
    ticks(&mut w, 160);
    w.note_tool_result_event(&second, "cargo", "error: 3 warnings", outcome(false));
    ticks(&mut w, 10);
    let broken = w
        .overworld_scene()
        .wayfarers
        .first()
        .map(|f| (f.x, f.y - 8.0));
    shot(&w, "d06_trial_failed.ppm", broken);
    ticks(&mut w, 1_200);
    shot(&w, "d07_lists_record.ppm", Some(at(Place::Lists)));

    let fetch = call(&mut w, "web_fetch", "url=https://docs.rs/ratatui");
    ticks(&mut w, 12);
    shot(&w, "d08_courier_out.ppm", Some(at(Place::Gatehouse)));
    ticks(&mut w, 200);
    w.note_tool_result_event(&fetch, "web_fetch", "ok", outcome(true));
    ticks(&mut w, 30);
    shot(&w, "d09_courier_home.ppm", Some(at(Place::Gatehouse)));

    for k in 0..3 {
        let commit = call(&mut w, "shell", "git commit -m 'deed'");
        ticks(&mut w, 40);
        if k == 2 {
            shot(&w, "d10_raven_aloft.ppm", Some(at(Place::Rookery)));
        }
        w.note_tool_result_event(&commit, "shell", "ok", outcome(true));
        ticks(&mut w, 400);
    }
    shot(&w, "d11_ravens_perched.ppm", Some(at(Place::Rookery)));

    let authored = |tx: i32, ty: i32| {
        let (x, y) = map::place_tile(tx, ty);
        ((x * TILE + 8) as f32, (y * TILE + 8) as f32)
    };

    // Searches: the hands in the fields, stooks, messengers on the road.
    let seeks: Vec<_> = (0..3)
        .map(|k| call(&mut w, "grep", &format!("pattern=needle{k}, path=cockpit")))
        .collect();
    ticks(&mut w, 200);
    shot(&w, "d12_fields_at_work.ppm", Some(authored(39, 26)));
    for (k, seek) in seeks.iter().enumerate() {
        w.note_tool_result_event(seek, "grep", "ok", outcome(k != 2));
        ticks(&mut w, 25);
    }
    ticks(&mut w, 120);
    let runner = w
        .overworld_scene()
        .wayfarers
        .iter()
        .find(|f| f.carrying)
        .map(|f| (f.x, f.y - 8.0));
    shot(&w, "d13_messengers.ppm", runner);
    for k in 0..6 {
        let seek = call(&mut w, "rg_search", &format!("query=more{k}"));
        ticks(&mut w, 5);
        w.note_tool_result_event(&seek, "rg_search", "ok", outcome(true));
    }
    ticks(&mut w, 900);
    shot(&w, "d14_fields_stooks.ppm", Some(authored(39, 26)));

    // Errands: villagers carry sacks to the granary.
    let errands: Vec<_> = ["ls -la", "python3 x.py", "du -sh target"]
        .into_iter()
        .map(|cmd| call(&mut w, "shell", cmd))
        .collect();
    ticks(&mut w, 50);
    shot(&w, "d15_village_errands.ppm", Some(authored(23, 27)));
    for (k, errand) in errands.iter().enumerate() {
        w.note_tool_result_event(errand, "shell", "ok", outcome(k != 1));
    }
    for k in 0..6 {
        let errand = call(&mut w, "shell", &format!("echo {k}"));
        ticks(&mut w, 30);
        w.note_tool_result_event(&errand, "shell", "ok", outcome(true));
    }
    ticks(&mut w, 60);
    shot(&w, "d16_village_granary.ppm", Some(authored(25, 28)));

    // Memory: a monk at vigil, candles on the step.
    let recall = call(&mut w, "memory_recall", "query=bench pin");
    ticks(&mut w, 60);
    shot(&w, "d17_chapel_vigil.ppm", Some(at(Place::Chapel)));
    w.note_tool_result_event(&recall, "memory_recall", "ok", outcome(true));
    for k in 0..4 {
        let kept = call(&mut w, "memory_deposit", &format!("key=note{k}"));
        ticks(&mut w, 10);
        w.note_tool_result_event(&kept, "memory_deposit", "ok", outcome(true));
    }
    ticks(&mut w, 400);
    shot(&w, "d18_chapel_candles.ppm", Some(at(Place::Chapel)));

    // Research: the glass sweeps, stars are fixed, owls fly.
    let study = call(&mut w, "arxiv", "query=sparse attention");
    ticks(&mut w, 40);
    shot(&w, "d19_observatory_beam.ppm", Some(at(Place::Observatory)));
    w.note_tool_result_event(&study, "arxiv", "ok", outcome(true));
    for k in 0..6 {
        let more = call(&mut w, "openalex", &format!("query=q{k}"));
        ticks(&mut w, 8);
        w.note_tool_result_event(&more, "openalex", "ok", outcome(true));
    }
    ticks(&mut w, 60);
    shot(
        &w,
        "d20_observatory_stars.ppm",
        Some(at(Place::Observatory)),
    );
    let owl = w
        .overworld_scene()
        .wayfarers
        .iter()
        .find(|f| f.errand == deeds::Errand::Owl)
        .map(|f| (f.x, f.y));
    shot(&w, "d21_owl_in_flight.ppm", owl);
    ticks(&mut w, 1_200);
    save(
        "d22_realm_after.ppm",
        &render_view(&w.overworld_scene(), View::realm()),
    );

    // A trial as a film: the wagon out, the verdict, the wagon home.
    let film = call(&mut w, "shell", "cargo test --workspace");
    for f in 0..90u32 {
        if f == 60 {
            w.note_tool_result_event(&film, "shell", "ok", outcome(true));
        }
        ticks(&mut w, 24);
        let focus = w
            .overworld_scene()
            .wayfarers
            .first()
            .map(|wagon| (wagon.x, wagon.y - 8.0));
        let mut s = w.overworld_scene();
        if let Some(focus) = focus {
            s.camera = focus;
        }
        crate::stage::world_viz::overworld::pace(&mut s, false);
        save(&format!("film_{f:03}.ppm"), &frame_sized(&s, 256, 160));
    }
}
