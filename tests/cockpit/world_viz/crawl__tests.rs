use super::*;

fn clock(region: Region, iteration: usize, elapsed: u32, work: Work) -> Clock {
    Clock {
        region,
        kind: LoopKind::Coding,
        iteration,
        elapsed,
        work,
        party: 3,
        danger: 0,
        fight: None,
        won: true,
        bouts: 0,
        found: None,
    }
}

#[test]
fn every_region_lays_out_a_floor_the_party_can_walk_end_to_end() {
    for region in [
        Region::TheMines,
        Region::DarkForest,
        Region::Swamp,
        Region::DragonKeep,
        Region::Homecoming,
    ] {
        for index in 0..4 {
            let plan = plan(region, LoopKind::Coding, index);
            assert!(plan.route.len() >= 2, "{region:?} {index}: a route");
            for pair in plan.spots.windows(2) {
                assert!(
                    plan.dungeon.path(pair[0], pair[1]).is_some(),
                    "{region:?} floor {index}: no way from {:?} to {:?}",
                    pair[0],
                    pair[1]
                );
            }
        }
    }
}

#[test]
fn a_turn_of_the_loop_walks_to_the_next_hall_and_stops_there() {
    let start = clock(Region::TheMines, 1, 0, Work::Forge);
    let plan = plan(start.region, start.kind, 0);
    let first = pose(&plan, &start);
    assert!(first.walking.is_some(), "setting out");
    let there = pose(
        &plan,
        &Clock {
            elapsed: 40 * 60,
            ..start
        },
    );
    assert!(there.walking.is_none(), "arrived within a minute");
    let (sx, sy) = party::centre(plan.spots[1]);
    assert!((there.eye.x - sx).abs() < 0.01 && (there.eye.y - sy).abs() < 0.01);
}

#[test]
fn frames_are_the_same_for_the_same_clock_and_move_with_it() {
    let at = clock(Region::DarkForest, 2, 400, Work::Rest);
    let a = frame(&at, (96, 72));
    assert_eq!(a, frame(&at, (96, 72)));
    let walking = clock(Region::DarkForest, 2, 40, Work::Rest);
    assert_ne!(a, frame(&walking, (96, 72)));
}

#[test]
fn the_log_says_what_the_party_is_doing() {
    let fight = log(
        &clock(Region::TheMines, 1, 40 * 60 + 200, Work::Forge),
        None,
    );
    assert!(!fight[0].is_empty());
    let walk = log(&clock(Region::TheMines, 1, 20, Work::Forge), None);
    assert!(walk[0].contains("leads the way"), "{walk:?}");
    let rest = log(&clock(Region::TheMines, 1, 40 * 60, Work::Rest), None);
    assert!(rest[0].contains("rests"), "{rest:?}");
    // A knight alone is named, not called a party.
    let alone = log(
        &Clock {
            party: 1,
            ..clock(Region::TheMines, 1, 40 * 60, Work::Rest)
        },
        None,
    );
    assert!(alone[0].starts_with("Sir Percival rests"), "{alone:?}");
}

/// Crawl frames for a look: `ANGEL_CRAWL_SHOTS=dir cargo test --bin angel -- --ignored write_crawl_shots`
#[test]
#[ignore]
fn write_crawl_shots() {
    let Some(dir) = std::env::var_os("ANGEL_CRAWL_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shots = [
        ("mines-walk", clock(Region::TheMines, 1, 60, Work::Forge)),
        (
            "mines-fight",
            clock(Region::TheMines, 1, 40 * 60 + 120, Work::Forge),
        ),
        (
            "forest-rest",
            clock(Region::DarkForest, 2, 40 * 60, Work::Rest),
        ),
        (
            "archive-study",
            clock(Region::Swamp, 3, 40 * 60, Work::Study),
        ),
        (
            "keep-council",
            clock(Region::DragonKeep, 1, 40 * 60, Work::Council),
        ),
        (
            "mines-scout",
            clock(Region::TheMines, 2, 40 * 60 + 90, Work::Dispatch),
        ),
        (
            "keep-guardian",
            clock(Region::DragonKeep, 5, 40 * 60 + 120, Work::Forge),
        ),
    ];
    // A treasure hall on some floor of the Mines, and a stair just taken.
    let treasure = (0..60usize)
        .find(|&i| {
            let plan = plan(Region::TheMines, LoopKind::Coding, i / party::STATIONS);
            let k = (i % party::STATIONS).min(plan.route.len() - 1);
            plan.dungeon.halls[plan.route[k]].kind == RoomKind::Treasure
        })
        .expect("a treasure hall within ten floors");
    let shots: Vec<(&str, Clock)> = shots
        .into_iter()
        .chain([
            (
                "mines-treasure",
                clock(Region::TheMines, treasure, 40 * 60, Work::Rest),
            ),
            (
                "mines-descent",
                clock(Region::TheMines, party::STATIONS, 24, Work::Rest),
            ),
        ])
        .collect();
    // Connected site shots use the production settlement renderer and actual
    // event transitions, not alternate art or a frame-time progression script.
    let mut site = crate::drive::together_settlement::Site::new(77, "Miner");
    for (name, receipts) in [
        ("settlement-player-hall", 0),
        ("settlement-first-cut", 1),
        ("settlement-stockpile", 9),
        ("settlement-workshop", 14),
        ("settlement-quarters", 19),
    ] {
        for n in 1..=receipts {
            site.apply("visual-fixture", n, true).unwrap();
        }
        let scene = settlement::Scene::of(&site);
        let img = scene.frame(3600, (288, 160));
        image::imageops::resize(&img, 864, 480, image::imageops::FilterType::Nearest)
            .save(dir.join(format!("{name}.png")))
            .unwrap();
        println!(
            "{name}: {} · {} stone · {} ore · {} tools",
            site.activity(),
            site.resources.stone,
            site.resources.ore,
            site.resources.tools
        );
    }
    for (name, at) in shots {
        let img = frame(&at, (288, 160));
        image::imageops::resize(&img, 288 * 3, 160 * 3, image::imageops::FilterType::Nearest)
            .save(dir.join(format!("{name}.png")))
            .unwrap();
        let said = log(&at, None);
        println!("{name}: {} | {}", said[0], said[1]);
    }
}

/// The braille a terminal shows for a frame, drawn as dots.
fn terminal_preview(cells: &crate::ui::term::art::ColoredBrailleImage) -> image::RgbaImage {
    let mut output = image::RgbaImage::from_pixel(
        cells.width as u32 * 8,
        cells.height as u32 * 16,
        image::Rgba([10, 10, 16, 255]),
    );
    for y in 0..cells.height {
        for x in 0..cells.width {
            let cell = cells.cells[y * cells.width + x];
            let bits = (cell.glyph as u32).saturating_sub(0x2800) as u8;
            for dy in 0..4 {
                for dx in 0..2 {
                    if bits & crate::ui::term::art::braille_dot_bit(dx, dy) == 0 {
                        continue;
                    }
                    for py in 0..3 {
                        for px in 0..3 {
                            output.put_pixel(
                                (x * 8 + dx * 4 + 1 + px) as u32,
                                (y * 16 + dy * 4 + 1 + py) as u32,
                                image::Rgba([cell.fg[0], cell.fg[1], cell.fg[2], 255]),
                            );
                        }
                    }
                }
            }
        }
    }
    output
}

/// A real world in a coding loop, its scryglass pane showing the crawl.
fn looping() -> crate::stage::world_viz::World {
    use crate::stage::world_viz::{AdventureEvent, Building, World};
    let mut world = World::new(42);
    world.settle_at_for_test(Building::Keep);
    world.note_adventure(AdventureEvent::LoopStarted {
        kind: LoopKind::Coding,
        task: "an actual coding loop".into(),
    });
    world.note_adventure(AdventureEvent::Iteration { n: 1 });
    world
}

#[test]
fn a_loop_shows_the_crawl_in_the_scryglass_and_says_what_happens() {
    use crate::ui::viz::lifecycle_viz::MotionMode;
    let world = looping();
    assert!(world.live_adventure_view());
    let clock = world.crawl_clock(MotionMode::Full);
    assert_eq!(clock.region, Region::TheMines);
    let cells = world
        .scryglass_frame_with_motion(48, 18, false, 0.0, 0.0, 1.05, MotionMode::Full)
        .expect("a frame");
    assert!(
        cells.cells.iter().any(|c| c.glyph != '\u{2800}'),
        "something drawn"
    );
    assert!(!world.crawl_log(MotionMode::Full)[0].is_empty());
}

/// What the terminal shows through a turn of a coding loop:
/// `ANGEL_CRAWL_SHOTS=dir cargo test --bin angel -- --ignored write_crawl_terminal_shots`
#[test]
#[ignore]
fn write_crawl_terminal_shots() {
    use crate::agent::harness::{ExecutionOutcome, ToolEventId, ToolOutcome, VerificationOutcome};
    use crate::ui::viz::lifecycle_viz::MotionMode;
    let Some(dir) = std::env::var_os("ANGEL_CRAWL_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut world = looping();
    let done = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::NotApplicable,
    };
    for frame in 0..160u32 {
        if frame == 80 {
            world.note_tool_call_event(
                ToolEventId("crawl-forge".into()),
                "write_file",
                "src/main.rs",
            );
        }
        if frame == 130 {
            world.note_tool_result_event(
                &ToolEventId("crawl-forge".into()),
                "write_file",
                "wrote",
                done,
            );
            world.note_tool_call_event(
                ToolEventId("crawl-study".into()),
                "read_file",
                "src/lib.rs",
            );
        }
        if frame % 20 == 0 {
            for (cols, rows) in [(48, 18), (96, 30)] {
                let cells = world
                    .scryglass_frame_with_motion(
                        cols,
                        rows,
                        false,
                        0.0,
                        0.0,
                        1.05,
                        MotionMode::Full,
                    )
                    .unwrap();
                terminal_preview(&cells)
                    .save(dir.join(format!("term-{cols}x{rows}-{frame:03}.png")))
                    .unwrap();
            }
            let said = world.crawl_log(MotionMode::Full);
            println!("{frame:03}: {} | {}", said[0], said[1]);
        }
        for _ in 0..8 {
            world.tick();
        }
    }
}

/// How long a frame takes at a Kitty pane's size (printed, not asserted).
#[test]
#[ignore]
fn time_a_crawl_frame() {
    let at = clock(Region::TheMines, 1, 40 * 60 + 120, Work::Forge);
    let _ = frame(&at, (300, 200));
    let start = std::time::Instant::now();
    for k in 0..20 {
        let _ = frame(
            &Clock {
                elapsed: at.elapsed + k * 4,
                ..at
            },
            (300, 200),
        );
    }
    println!(
        "crawl frame 300x200: {:.2} ms",
        start.elapsed().as_secs_f64() * 1000.0 / 20.0
    );
}

/// A record's guardian keeps the pane even once the loop is home, says
/// what's happening as the fight goes, and is gone after three minutes.
#[test]
fn a_promoted_record_brings_a_guardian_the_party_beats() {
    use crate::stage::world_viz::AdventureEvent;
    use crate::ui::viz::lifecycle_viz::MotionMode;
    let mut world = looping();
    world.note_adventure(AdventureEvent::RecordPromoted {
        score: Some("948.94M".into()),
    });
    world.note_adventure(AdventureEvent::LoopFinished { ok: true });
    let lines = |world: &crate::stage::world_viz::World| world.crawl_log(MotionMode::Full);
    assert!(world.live_adventure_view());
    assert!(lines(&world)[0].contains("record"), "{:?}", lines(&world));
    let mut said = std::collections::BTreeSet::new();
    for _ in 0..(40 * 180 / 8) {
        for _ in 0..8 {
            world.tick();
        }
        said.insert(lines(&world)[0].clone());
        if world.crawl_clock(MotionMode::Full).fight.is_none() {
            break;
        }
    }
    assert!(
        said.iter().any(|l| l.contains("strikes")),
        "the blows: {said:?}"
    );
    assert!(
        said.iter().any(|l| l.contains("falls")),
        "the fall: {said:?}"
    );
    assert!(
        said.iter().any(|l| l.contains("light of a new record")),
        "the light after: {said:?}"
    );
    for _ in 0..80 {
        world.tick();
    }
    assert!(
        world.crawl_clock(MotionMode::Full).fight.is_none(),
        "over after three minutes"
    );
}

/// The fight goes back and forth: the guardian's health falls as the party
/// presses, climbs as it rallies; won, it ends at nothing; lost, whole.
#[test]
fn the_fight_goes_back_and_forth() {
    let won: Vec<f32> = (0..7200)
        .step_by(40)
        .map(|t| fight::health_for_test(true, t))
        .collect();
    assert!(won.windows(2).any(|w| w[1] > w[0] + 0.001), "it rallies");
    assert!(won.windows(2).any(|w| w[1] < w[0] - 0.001), "it's pressed");
    assert!(*won.last().unwrap() < 0.01);
    let lost = fight::health_for_test(false, 4700);
    assert!(lost > 0.99, "lost, it's whole: {lost}");
}

/// A promotion that didn't come: the guardian holds, and the party falls
/// back; two minutes, then the crawl again.
#[test]
fn a_promotion_that_didnt_come_is_a_retreat() {
    use crate::stage::world_viz::AdventureEvent;
    use crate::ui::viz::lifecycle_viz::MotionMode;
    let mut world = looping();
    world.note_adventure(AdventureEvent::PromotionFailed {
        score: Some("951.02M".into()),
    });
    let mut said = std::collections::BTreeSet::new();
    for _ in 0..(40 * 120 / 8) {
        for _ in 0..8 {
            world.tick();
        }
        said.insert(world.crawl_log(MotionMode::Full)[0].clone());
    }
    assert!(said.iter().any(|l| l.contains("falls back")), "{said:?}");
    assert!(said.iter().any(|l| l.contains("not a record")), "{said:?}");
    assert!(!said.iter().any(|l| l.contains("light of a new record")));
    for _ in 0..40 {
        world.tick();
    }
    assert!(world.crawl_clock(MotionMode::Full).fight.is_none());
}

/// The fight, frame by frame: `ANGEL_CRAWL_SHOTS=dir cargo test --bin angel -- --ignored write_record_fight_shots`
#[test]
#[ignore]
fn write_record_fight_shots() {
    let Some(dir) = std::env::var_os("ANGEL_CRAWL_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, region, t, won) in [
        ("fight-0-tremble", Region::TheMines, 80, true),
        ("fight-1-rising", Region::TheMines, 300, true),
        ("fight-2-blows", Region::TheMines, 1200, true),
        ("fight-3-lash", Region::TheMines, 480 + 220 * 5 + 15, true),
        ("fight-4-rally", Region::TheMines, 2300, true),
        ("fight-5-mend", Region::Swamp, 4560, true),
        ("fight-6-rage", Region::Swamp, 6100, true),
        ("fight-7-falls", Region::Swamp, 6700, true),
        ("fight-8-light", Region::Swamp, 7100, true),
        ("retreat-1-whole", Region::DragonKeep, 3600, false),
        ("retreat-2-backing", Region::DragonKeep, 4300, false),
        ("retreat-3-dark", Region::DragonKeep, 4700, false),
    ] {
        let at = Clock {
            fight: Some(t),
            won,
            bouts: 1,
            ..clock(region, 1, 40 * 60, Work::Forge)
        };
        let img = frame(&at, (288, 160));
        image::imageops::resize(&img, 288 * 3, 160 * 3, image::imageops::FilterType::Nearest)
            .save(dir.join(format!("{name}.png")))
            .unwrap();
        let said = log(&at, Some("948.94M"));
        println!("{name}: {} | {}", said[0], said[1]);
    }
}

/// A record's fight as a terminal shows it:
/// `ANGEL_CRAWL_SHOTS=dir cargo test --bin angel -- --ignored write_fight_terminal_shots`
#[test]
#[ignore]
fn write_fight_terminal_shots() {
    use crate::stage::world_viz::AdventureEvent;
    use crate::ui::viz::lifecycle_viz::MotionMode;
    let Some(dir) = std::env::var_os("ANGEL_CRAWL_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut world = looping();
    world.note_adventure(AdventureEvent::RecordPromoted {
        score: Some("948.94M".into()),
    });
    for k in 0..=7200u32 {
        if [300, 1200, 2300, 6100, 7100].contains(&k) {
            let cells = world
                .scryglass_frame_with_motion(96, 30, false, 0.0, 0.0, 1.05, MotionMode::Full)
                .unwrap();
            terminal_preview(&cells)
                .save(dir.join(format!("term-fight-{k:04}.png")))
                .unwrap();
        }
        world.tick();
    }
}

/// A preview film of the crawl, frame by frame, with the two lines under
/// each: a walk to a hall, each kind of work in it, a record's fight at 4x
/// and a retreat's end at 8x.
/// `ANGEL_CRAWL_FILM=dir cargo test --bin angel -- --ignored write_crawl_film`
#[test]
#[ignore]
fn write_crawl_film() {
    use crate::agent::harness::{ExecutionOutcome, ToolEventId, ToolOutcome, VerificationOutcome};
    use crate::stage::world_viz::AdventureEvent;
    use crate::ui::viz::lifecycle_viz::MotionMode;
    let Some(dir) = std::env::var_os("ANGEL_CRAWL_FILM") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::NotApplicable,
    };
    let mut world = looping();
    // A formation of three, so the party shows.
    world.note_adventure(AdventureEvent::Party { size: 3 });
    let mut frames: Vec<(String, String, String)> = Vec::new();
    let mut shoot = |world: &crate::stage::world_viz::World, part: &str| {
        let clock = world.crawl_clock(MotionMode::Full);
        let img = frame(&clock, (288, 160));
        let name = format!("{:05}.png", frames.len());
        img.save(dir.join(&name)).unwrap();
        let said = world.crawl_log(MotionMode::Full);
        frames.push((part.to_string(), said[0].clone(), said[1].clone()));
    };
    // The walk to the next hall, at twice the pace.
    for _ in 0..200 {
        if !world.crawl_log(MotionMode::Full)[0].contains("leads the way") {
            break;
        }
        shoot(&world, "walk");
        for _ in 0..8 {
            world.tick();
        }
    }
    // Each kind of work in the hall, in real time.
    let mut call = 0;
    for (tool, seconds) in [
        (None, 4),
        (Some("write_file"), 12),
        (Some("read_file"), 6),
        (Some("delegate"), 6),
        (Some("web_fetch"), 6),
    ] {
        call += 1;
        let id = ToolEventId(format!("film-{call}"));
        if let Some(tool) = tool {
            world.note_tool_call_event(id.clone(), tool, "src/main.rs");
        }
        for _ in 0..seconds * 10 {
            shoot(&world, tool.unwrap_or("rest"));
            for _ in 0..4 {
                world.tick();
            }
        }
        if let Some(tool) = tool {
            world.note_tool_result_event(&id, tool, "done", done);
        }
    }
    // A record: the fight, at four times its pace.
    world.note_adventure(AdventureEvent::RecordPromoted {
        score: Some("948.94M".into()),
    });
    while world.crawl_clock(MotionMode::Full).fight.is_some() {
        shoot(&world, "record");
        for _ in 0..16 {
            world.tick();
        }
    }
    // Another day: a promotion that didn't come, its last minute at eight
    // times.
    world.note_adventure(AdventureEvent::PromotionFailed {
        score: Some("951.02M".into()),
    });
    for _ in 0..40 * 60 {
        world.tick();
    }
    while world.crawl_clock(MotionMode::Full).fight.is_some() {
        shoot(&world, "retreat");
        for _ in 0..32 {
            world.tick();
        }
    }
    let captions: Vec<serde_json::Value> = frames
        .iter()
        .map(|(part, a, b)| serde_json::json!({"part": part, "lines": [a, b]}))
        .collect();
    std::fs::write(
        dir.join("captions.json"),
        serde_json::to_string(&captions).unwrap(),
    )
    .unwrap();
}

/// The crawl as a small corner pane shows it (braille cells):
/// `ANGEL_CRAWL_SHOTS=dir cargo test --bin angel -- --ignored write_corner_shots`
#[test]
#[ignore]
fn write_corner_shots() {
    use crate::stage::world_viz::AdventureEvent;
    use crate::ui::viz::lifecycle_viz::MotionMode;
    let Some(dir) = std::env::var_os("ANGEL_CRAWL_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let party = |fight: bool| {
        let mut world = looping();
        world.note_adventure(AdventureEvent::Party { size: 3 });
        for _ in 0..40 * 40 {
            world.tick();
        }
        if fight {
            world.note_adventure(AdventureEvent::RecordPromoted { score: None });
            for _ in 0..1200 {
                world.tick();
            }
        }
        world
    };
    let (calm, fighting) = (party(false), party(true));
    for (name, w) in [("calm", &calm), ("fight", &fighting)] {
        for (cols, rows) in [(32, 10), (40, 13), (56, 16)] {
            let cells = w
                .scryglass_frame_with_motion(cols, rows, false, 0.0, 0.0, 1.05, MotionMode::Full)
                .unwrap();
            terminal_preview(&cells)
                .save(dir.join(format!("corner-{name}-{cols}x{rows}.png")))
                .unwrap();
        }
    }
}

/// A measurement coming in is a chest the party prises open, for ten
/// seconds; a stalled loop's torch gutters.
#[test]
fn a_measurement_is_a_chest_and_a_stall_gutters_the_torch() {
    use crate::stage::world_viz::AdventureEvent;
    use crate::ui::viz::lifecycle_viz::MotionMode;
    let mut world = looping();
    for _ in 0..40 * 60 {
        world.tick();
    }
    world.note_adventure(AdventureEvent::MeasurementObserved);
    world.tick();
    assert!(
        world.crawl_log(MotionMode::Full)[0].contains("prises open a chest"),
        "{:?}",
        world.crawl_log(MotionMode::Full)
    );
    for _ in 0..400 {
        world.tick();
    }
    assert!(!world.crawl_log(MotionMode::Full)[0].contains("chest"));
    let calm = clock(Region::TheMines, 1, 40 * 60, Work::Rest);
    let stalled = Clock { danger: 2, ..calm };
    assert!(log(&stalled, None)[1].contains("gutters"));
    assert_ne!(frame(&calm, (96, 72)), frame(&stalled, (96, 72)));
}

/// A chest found, and a stalled loop's dark:
/// `ANGEL_CRAWL_SHOTS=dir cargo test --bin angel -- --ignored write_loop_event_shots`
#[test]
#[ignore]
fn write_loop_event_shots() {
    let Some(dir) = std::env::var_os("ANGEL_CRAWL_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rest = clock(Region::TheMines, 1, 40 * 60, Work::Rest);
    for (name, at) in [
        (
            "found-closed",
            Clock {
                found: Some(20),
                ..rest
            },
        ),
        (
            "found-open",
            Clock {
                found: Some(120),
                ..rest
            },
        ),
        ("stall-0", rest),
        ("stall-2", Clock { danger: 2, ..rest }),
        (
            "stall-3-walk",
            Clock {
                danger: 3,
                elapsed: 60,
                ..rest
            },
        ),
    ] {
        let img = frame(&at, (288, 160));
        image::imageops::resize(&img, 288 * 3, 160 * 3, image::imageops::FilterType::Nearest)
            .save(dir.join(format!("{name}.png")))
            .unwrap();
        let said = log(&at, None);
        println!("{name}: {} | {}", said[0], said[1]);
    }
}

/// A sconce and a banner on the wall, close: `ANGEL_CRAWL_SHOTS=dir cargo test --bin angel -- --ignored write_fixture_shots`
#[test]
#[ignore]
fn write_fixture_shots() {
    let Some(dir) = std::env::var_os("ANGEL_CRAWL_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let plan = plan(Region::TheMines, LoopKind::Coding, 0);
    let faces = textures::faces();
    let decals = wall_decals(0);
    let kit = render::Kit {
        walls: &faces.walls,
        decals: &decals,
        floor: &faces.floor,
        water: &faces.water,
        lava: &faces.lava,
        stairs: &faces.stairs,
        ceiling: &faces.ceiling,
    };
    let mut hung: Vec<((i32, i32, i8, i8), u8)> = plan
        .dungeon
        .grid
        .decals
        .iter()
        .map(|(k, v)| (*k, *v))
        .collect();
    hung.sort();
    for kind in [dungeon::SCONCE, dungeon::BANNER] {
        let Some(((x, y, dx, dy), _)) = hung.iter().copied().find(|((x, y, dx, dy), k)| {
            // Room for the eye to stand two cells off the face.
            *k == kind
                && plan
                    .dungeon
                    .grid
                    .open(x + i32::from(*dx) * 2, y + i32::from(*dy) * 2)
        }) else {
            continue;
        };
        let eye = render::Eye {
            x: x as f32 + 0.5 + f32::from(dx) * 2.2,
            y: y as f32 + 0.5 + f32::from(dy) * 2.2,
            heading: (-f32::from(dy)).atan2(-f32::from(dx)),
            fov: 1.15,
            z: 0.5,
            pitch: 0.0,
        };
        let torch = render::Torch {
            reach: 5.5,
            gain: 1.0,
            glow: None,
            sconces: sconces_near(&plan, (eye.x, eye.y)),
        };
        let img = render::render(&plan.dungeon.grid, &eye, &[], &kit, torch, (288, 160));
        image::imageops::resize(&img, 288 * 3, 160 * 3, image::imageops::FilterType::Nearest)
            .save(dir.join(format!("fixture-{kind}.png")))
            .unwrap();
    }
}
